$(document).ready(function() {
  // Получить список ребер графа из JSON-объекта
  var edges = JSON.parse($("#graph").data("edges"));

  // Создать список элементов графа
  var graph = [];
  for (var i = 0; i < edges.length; i++) {
    var edge = edges[i];
    var from = edge["from"];
    var to = edge["to"];
    graph.push("<li><a href='#'>" + from + " -> " + to + "</a></li>");
  }

  // Добавить список элементов графа в DOM
  $("#graph").html(graph);
});
